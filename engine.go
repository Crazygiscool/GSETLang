package main

import "strings"

type GSETConfig struct {
	Keywords map[string]string //Keywords var with the type of map and input string, output string. pair
}

func ParseGSet(src string) (GSETConfig, string){ //output only defined the type of outputs

	conf := GSETConfig{Keywords: make(map[string]string)}
	
	parts := strings.SplitN(src,"---",2)
	
	
	//if code part after split is lesser than 2
	if len(parts) < 2 {
		//return config, which is blank, and the spurce code
		return conf, src
	
	}
	
	return conf, body[1] //not empty, so we return config and second part, the code body

}

